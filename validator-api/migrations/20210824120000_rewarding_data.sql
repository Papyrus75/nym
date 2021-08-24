-- for each epoch there shall be a summary
CREATE TABLE rewarding_report
(
    id                           INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    timestamp                    INTEGER NOT NULL,

    eligible_mixnodes            INTEGER NOT NULL,
    eligible_gateways            INTEGER NOT NULL,

    possibly_unrewarded_mixnodes INTEGER NOT NULL,
    possibly_unrewarded_gateways INTEGER NOT NULL
);

-- containing possibly many (ideally zero!) failed reward entries
-- (this refers to a reward chunk)
CREATE TABLE failed_mixnode_reward_chunk
(
    id                INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    error_message     VARCHAR NOT NULL,

    reward_summary_id INTEGER NOT NULL,

    FOREIGN KEY (reward_summary_id) REFERENCES rewarding_report (id)
);


CREATE TABLE failed_gateway_reward_chunk
(
    id                INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    error_message     VARCHAR NOT NULL,

    reward_summary_id INTEGER NOT NULL,

    FOREIGN KEY (reward_summary_id) REFERENCES rewarding_report (id)
);


-- and each such failed_mixnode_reward_chunk contain mixnodes that might have been unrewarded
-- (but we don't know for sure - at least in typescript we could have gotten a timeout yet the tx still was executed)
-- this table only exists because sqlite has no arrays
CREATE TABLE possibly_unrewarded_mixnode
(
    id                             INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    identity                       VARCHAR NOT NULL,
    uptime                         INTEGER NOT NULL,

    failed_mixnode_reward_chunk_id INTEGER NOT NULL,

    FOREIGN KEY (failed_mixnode_reward_chunk_id) REFERENCES failed_mixnode_reward_chunk (id)
);


CREATE TABLE possibly_unrewarded_gateway
(
    id                             INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    identity                       VARCHAR NOT NULL,
    uptime                         INTEGER NOT NULL,

    failed_gateway_reward_chunk_id INTEGER NOT NULL,

    FOREIGN KEY (failed_gateway_reward_chunk_id) REFERENCES failed_gateway_reward_chunk (id)
)