CREATE TABLE commit_account (id BIGINT NOT NULL PRIMARY KEY, reserved BIGINT NOT NULL, confirmed BIGINT NOT NULL) ENGINE=InnoDB;
INSERT INTO commit_account(id,reserved,confirmed) VALUES (1,0,0);
CREATE TABLE commit_reservation (xid VARCHAR(100) NOT NULL, branch_id BIGINT NOT NULL, phase VARCHAR(16) NOT NULL, effect_count INT NOT NULL, PRIMARY KEY(xid,branch_id)) ENGINE=InnoDB;
