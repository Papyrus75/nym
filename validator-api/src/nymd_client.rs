// Copyright 2021 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::rewarding::{
    error::RewardingError, GatewayToReward, MixnodeToReward, GATEWAY_REWARD_OP_BASE_GAS_LIMIT,
    MIXNODE_REWARD_OP_BASE_GAS_LIMIT, PER_GATEWAY_DELEGATION_GAS_INCREASE,
    PER_MIXNODE_DELEGATION_GAS_INCREASE,
};
use config::defaults::DEFAULT_VALIDATOR_API_PORT;
use mixnet_contract::{Delegation, ExecuteMsg, GatewayBond, IdentityKey, MixNodeBond};
use std::sync::Arc;
use tokio::sync::RwLock;
use validator_client::nymd::{
    CosmWasmClient, Fee, QueryNymdClient, SigningCosmWasmClient, SigningNymdClient,
};
use validator_client::ValidatorClientError;

pub(crate) struct Client<C>(Arc<RwLock<validator_client::Client<C>>>);

impl<C> Clone for Client<C> {
    fn clone(&self) -> Self {
        Client(Arc::clone(&self.0))
    }
}

impl Client<QueryNymdClient> {
    pub(crate) fn new_query(config: &Config) -> Self {
        // the api address is irrelevant here as **WE ARE THE API**
        // and we won't be talking on the socket here.
        let api_url = format!("http://localhost:{}", DEFAULT_VALIDATOR_API_PORT)
            .parse()
            .unwrap();
        let nymd_url = config.get_nymd_validator_url();

        let mixnet_contract = config
            .get_mixnet_contract_address()
            .parse()
            .expect("the mixnet contract address is invalid!");

        let client_config = validator_client::Config::new(nymd_url, api_url, Some(mixnet_contract));
        let inner =
            validator_client::Client::new_query(client_config).expect("Failed to connect to nymd!");

        Client(Arc::new(RwLock::new(inner)))
    }
}

impl Client<SigningNymdClient> {
    pub(crate) fn new_signing(config: &Config) -> Self {
        // the api address is irrelevant here as **WE ARE THE API**
        // and we won't be talking on the socket here.
        let api_url = format!("http://localhost:{}", DEFAULT_VALIDATOR_API_PORT)
            .parse()
            .unwrap();
        let nymd_url = config.get_nymd_validator_url();

        let mixnet_contract = config
            .get_mixnet_contract_address()
            .parse()
            .expect("the mixnet contract address is invalid!");
        let mnemonic = config
            .get_mnemonic()
            .parse()
            .expect("the mnemonic is invalid!");

        let client_config = validator_client::Config::new(nymd_url, api_url, Some(mixnet_contract));
        let inner = validator_client::Client::new_signing(client_config, mnemonic)
            .expect("Failed to connect to nymd!");

        Client(Arc::new(RwLock::new(inner)))
    }
}

impl<C> Client<C> {
    pub(crate) async fn get_mixnodes(&self) -> Result<Vec<MixNodeBond>, ValidatorClientError>
    where
        C: CosmWasmClient + Sync,
    {
        self.0.read().await.get_all_nymd_mixnodes().await
    }

    pub(crate) async fn get_gateways(&self) -> Result<Vec<GatewayBond>, ValidatorClientError>
    where
        C: CosmWasmClient + Sync,
    {
        self.0.read().await.get_all_nymd_gateways().await
    }

    pub(crate) async fn get_mixnode_delegations(
        &self,
        identity: IdentityKey,
    ) -> Result<Vec<Delegation>, ValidatorClientError>
    where
        C: CosmWasmClient + Sync,
    {
        self.0
            .read()
            .await
            .get_all_nymd_mixnode_delegations(identity)
            .await
    }

    pub(crate) async fn get_gateway_delegations(
        &self,
        identity: IdentityKey,
    ) -> Result<Vec<Delegation>, ValidatorClientError>
    where
        C: CosmWasmClient + Sync,
    {
        self.0
            .read()
            .await
            .get_all_nymd_gateway_delegations(identity)
            .await
    }

    async fn estimate_mixnode_reward_fees(&self, nodes: usize, total_delegations: usize) -> Fee {
        let total_gas_limit = MIXNODE_REWARD_OP_BASE_GAS_LIMIT * nodes as u64
            + PER_MIXNODE_DELEGATION_GAS_INCREASE * total_delegations as u64;

        self.0
            .read()
            .await
            .nymd
            .calculate_custom_fee(total_gas_limit)
    }

    async fn estimate_gateway_reward_fees(&self, nodes: usize, total_delegations: usize) -> Fee {
        let total_gas_limit = GATEWAY_REWARD_OP_BASE_GAS_LIMIT * nodes as u64
            + PER_GATEWAY_DELEGATION_GAS_INCREASE * total_delegations as u64;

        self.0
            .read()
            .await
            .nymd
            .calculate_custom_fee(total_gas_limit)
    }

    pub(crate) async fn reward_mixnodes(
        &self,
        nodes: &[MixnodeToReward],
    ) -> Result<(), RewardingError>
    where
        C: SigningCosmWasmClient + Sync,
    {
        let total_delegations = nodes.iter().map(|node| node.total_delegations).sum();
        let fee = self
            .estimate_mixnode_reward_fees(nodes.len(), total_delegations)
            .await;
        let msgs: Vec<(ExecuteMsg, _)> = nodes
            .iter()
            .map(Into::into)
            .zip(std::iter::repeat(Vec::new()))
            .collect();

        let memo = format!("rewarding {} mixnodes", msgs.len());

        let contract = self
            .0
            .read()
            .await
            .get_mixnet_contract_address()
            .ok_or(RewardingError::UnspecifiedContractAddress)?;

        // technically we don't require a write lock here, however, we really don't want to be executing
        // multiple blocks concurrently as one of them WILL fail due to incorrect sequence number
        self.0
            .write()
            .await
            .nymd
            .execute_multiple(&contract, msgs, fee, memo)
            .await?;

        Ok(())
    }

    pub(crate) async fn reward_gateways(
        &self,
        nodes: &[GatewayToReward],
    ) -> Result<(), RewardingError>
    where
        C: SigningCosmWasmClient + Sync,
    {
        let total_delegations = nodes.iter().map(|node| node.total_delegations).sum();
        let fee = self
            .estimate_gateway_reward_fees(nodes.len(), total_delegations)
            .await;
        let msgs: Vec<(ExecuteMsg, _)> = nodes
            .iter()
            .map(Into::into)
            .zip(std::iter::repeat(Vec::new()))
            .collect();

        let memo = format!("rewarding {} gateways", msgs.len());

        let contract = self
            .0
            .read()
            .await
            .get_mixnet_contract_address()
            .ok_or(RewardingError::UnspecifiedContractAddress)?;

        // technically we don't require a write lock here, however, we really don't want to be executing
        // multiple blocks concurrently as one of them WILL fail due to incorrect sequence number
        self.0
            .write()
            .await
            .nymd
            .execute_multiple(&contract, msgs, fee, memo)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_status_api::models::Uptime;
    use mixnet_contract::{Coin, MixNode};
    use std::io::Read;
    use validator_client::nymd::cosmwasm_client::types::EmptyMsg;
    use validator_client::nymd::CosmosCoin;

    // #[tokio::test]
    // async fn upload() {
    //     let mnemonic = "claim border flee add vehicle crack romance assault fold wide flag year cousin false junk analyst parent eagle act visual tongue weasel basket impulse";
    //     let config = Config::default();
    //
    //     let client_config = validator_client::Config::new(
    //         config.get_nymd_validator_url(),
    //         config.get_all_validator_api_endpoints()[0].clone(),
    //         None,
    //     );
    //     let client =
    //         validator_client::Client::new_signing(client_config, mnemonic.parse().unwrap())
    //             .expect("Failed to connect to nymd!");
    //
    //     let mut file = std::fs::File::open(
    //         "../contracts/mixnet/target/wasm32-unknown-unknown/release/mixnet_contracts.wasm",
    //     )
    //     .unwrap();
    //     let mut data = Vec::new();
    //
    //     file.read_to_end(&mut data).unwrap();
    //
    //     let res = client.nymd.upload(data, "foomp!", None).await.unwrap();
    //     let res = client
    //         .nymd
    //         .instantiate(
    //             res.code_id,
    //             &EmptyMsg {},
    //             "mixnet contract".into(),
    //             "foomper",
    //             None,
    //         )
    //         .await
    //         .unwrap();
    //
    //     println!("contract: {:?}", res.contract_address);
    // }

    const contract: &str = "punk1256v8eljyhvspllhk4g393lx8ntnzf6yavap8k";

    fn make_client(mnemonic: &str) -> Client<SigningNymdClient> {
        let config = Config::default()
            .with_mnemonic(mnemonic)
            .with_custom_mixnet_contract(contract);

        Client::new_signing(&config)
    }

    use std::convert::TryFrom;

    #[tokio::test]
    async fn foo() {
        let mne1 = "believe stumble tide cage random slogan ranch genius engage kick lawsuit fashion chest entire announce tilt insect zebra jump live giant list gloom code";
        let addr1 = "punk1akuph5338zywytj6mq69e5klsl9g9ggkcnmrql"
            .parse()
            .unwrap();

        let mne2 = "eager receive install devote frost army valley theory inspire venture catch lava museum finish ostrich move glance dust top noble box insect portion glove";
        let addr2 = "punk1m3cp28uk6nfwq4cg3yagy24eunu25wldkq6xff"
            .parse()
            .unwrap();

        let admin_mnemonic = "claim border flee add vehicle crack romance assault fold wide flag year cousin false junk analyst parent eagle act visual tongue weasel basket impulse";
        let admin_addr = "punk1q9n5a3cgw3azegcddr82s0f5nxeel4pup8vxzt"
            .parse()
            .unwrap();

        let monitor_mnemonic = "carpet sugar verify very pumpkin still unusual else rebuild holiday oxygen dizzy about pattern involve profit know employ already leisure draw nation buddy congress";
        let monitor_addr = "punk1v9qauwdq5terag6uvfsdytcs2d0sdmfdy7hgk3"
            .parse()
            .unwrap();

        let admin = make_client(admin_mnemonic);
        let user1 = make_client(mne1);
        let user2 = make_client(mne2);
        let monitor = make_client(monitor_mnemonic);

        // let _100punks = CosmosCoin {
        //     denom: "upunk".parse().unwrap(),
        //     amount: 100_000_000u64.into(),
        // };

        let _1punk = Coin::new(1_000_000, "upunk");

        let _100punks = Coin::new(100_000_000, "upunk");

        let mix1 = MixNode {
            host: "1.1.1.1".to_string(),
            mix_port: 1,
            verloc_port: 2,
            http_api_port: 3,
            sphinx_key: "bbb".to_string(),
            identity_key: "bbb".to_string(),
            version: "0.11.0".to_string(),
        };

        let mix2 = MixNode {
            host: "1.1.1.2".to_string(),
            mix_port: 1,
            verloc_port: 2,
            http_api_port: 3,
            sphinx_key: "aaa".to_string(),
            identity_key: "aaa".to_string(),
            version: "0.11.0".to_string(),
        };

        // admin
        //     .0
        //     .read()
        //     .await
        //     .nymd
        //     .delegate_to_mixnode("aaa".to_owned(), _1punk)
        //     .await
        //     .unwrap();

        // user1
        //     .0
        //     .read()
        //     .await
        //     .nymd
        //     .bond_mixnode(mix1, _100punks.clone())
        //     .await
        //     .unwrap();
        // user2
        //     .0
        //     .read()
        //     .await
        //     .nymd
        //     .bond_mixnode(mix2, _100punks.clone())
        //     .await
        //     .unwrap();

        println!("{:?}", admin.0.read().await.nymd.get_balance(&addr1).await);
        println!("{:?}", admin.0.read().await.nymd.get_balance(&addr2).await);
        println!(
            "{:?}",
            admin.0.read().await.nymd.get_balance(&admin_addr).await
        );
        println!(
            "{:?}\n\n",
            admin.0.read().await.nymd.get_balance(&monitor_addr).await
        );

        let mixes = admin.get_mixnodes().await.unwrap();
        println!(
            "{:?} + {:?} and {:?} + {:?}",
            mixes[0].bond_amount,
            mixes[0].total_delegation,
            mixes[1].bond_amount,
            mixes[1].total_delegation
        );

        let rewarded = vec![
            MixnodeToReward {
                identity: "aaa".to_string(),
                uptime: Uptime::try_from(100u8).unwrap(),
                total_delegations: 0,
            },
            MixnodeToReward {
                identity: "bbb".to_string(),
                uptime: Uptime::try_from(100u8).unwrap(),
                total_delegations: 0,
            },
        ];

        monitor.reward_mixnodes(&rewarded).await.unwrap();

        let mixes = admin.get_mixnodes().await.unwrap();
        println!("{:?} and {:?}", mixes[0].bond_amount, mixes[1].bond_amount);
    }
}
