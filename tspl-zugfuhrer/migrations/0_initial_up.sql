-- tspl-zugfuhrer: initial schema

CREATE TABLE trains (
	id INTEGER PRIMARY KEY,
	tspl_id BLOB UNIQUE NOT NULL,
	parent_uid TEXT NOT NULL,
	parent_start_date TEXT NOT NULL,
	parent_stp_indicator TEXT NOT NULL,
	date TEXT NOT NULL,
	trust_id TEXT,
	darwin_rid TEXT UNIQUE,
	headcode TEXT,
	crosses_midnight BOOL NOT NULL,
	parent_source INT NOT NULL,
	UNIQUE(date, trust_id)
);

CREATE TABLE train_movements (
	id INTEGER PRIMARY KEY,
	parent_train INT NOT NULL REFERENCES trains ON DELETE CASCADE,
	updates INT REFERENCES train_movements ON DELETE CASCADE,
	tiploc TEXT NOT NULL,
	action INT NOT NULL,
	actual BOOL NOT NULL,
	time TEXT NOT NULL,
	public_time TEXT,
	day_offset INT NOT NULL,
	source INT NOT NULL,
	platform TEXT,
	pfm_suppr BOOL NOT NULL,
	unknown_delay BOOL NOT NULL,
	UNIQUE(parent_train, updates, tiploc, action, time, day_offset)
);

-- This is useful for when trains get deleted and it cascades;
-- also just for listing all the train movements for a train
CREATE INDEX train_movements_parent_train ON train_movements (parent_train);
-- ...likewise
CREATE INDEX train_movements_updates ON train_movements (updates);

