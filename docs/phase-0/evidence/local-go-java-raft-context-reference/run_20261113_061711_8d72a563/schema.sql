CREATE TABLE accounts (id BIGINT NOT NULL PRIMARY KEY, balance BIGINT NOT NULL) ENGINE=InnoDB;
INSERT INTO accounts (id,balance) VALUES (1,1000);
CREATE TABLE undo_log (
 id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
 branch_id BIGINT NOT NULL,
 xid VARCHAR(100) NOT NULL,
 context VARCHAR(128) NOT NULL,
 rollback_info LONGBLOB NOT NULL,
 log_status INT NOT NULL,
 log_created DATETIME NOT NULL,
 log_modified DATETIME NOT NULL,
 ext VARCHAR(100) DEFAULT NULL,
 UNIQUE KEY ux_undo_log (xid,branch_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE commit_account (id BIGINT NOT NULL PRIMARY KEY, reserved BIGINT NOT NULL, confirmed BIGINT NOT NULL) ENGINE=InnoDB;
INSERT INTO commit_account(id,reserved,confirmed) VALUES (1,0,0);
CREATE TABLE commit_reservation (xid VARCHAR(100) NOT NULL, branch_id BIGINT NOT NULL, phase VARCHAR(16) NOT NULL, effect_count INT NOT NULL, prepared_units BIGINT NOT NULL, prepared_token VARCHAR(64) NOT NULL, PRIMARY KEY(xid,branch_id)) ENGINE=InnoDB;
