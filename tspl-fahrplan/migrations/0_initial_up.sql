-- tspl-fahrplan: initial schema

CREATE TABLE schedules (
	id INTEGER PRIMARY KEY,
	tspl_id BLOB UNIQUE NOT NULL,
	uid TEXT NOT NULL,
	start_date TEXT NOT NULL,
	end_date TEXT NOT NULL,
	days INT NOT NULL,
	stp_indicator TEXT NOT NULL,
	signalling_id TEXT,
	source INT NOT NULL,
	file_metaseq INT,
	darwin_id TEXT,
	crosses_midnight BOOL NOT NULL,
	UNIQUE(uid, start_date, stp_indicator, source)
	-- (indexes implicit for primary key and UNIQUE constraints)
);

CREATE TABLE schedule_movements (
	id INTEGER PRIMARY KEY,
	parent_sched INT NOT NULL REFERENCES schedules ON DELETE CASCADE,
	tiploc TEXT NOT NULL,
	action INT NOT NULL,
	time TEXT NOT NULL,
	day_offset INT NOT NULL,
	platform TEXT,
	public_time TEXT
);

CREATE INDEX schedule_movements_parent_sched ON schedule_movements (parent_sched);
CREATE INDEX schedule_movements_tiploc_time ON schedule_movements (tiploc, time);

CREATE TABLE schedule_files (
	sequence INT NOT NULL,
	timestamp INT NOT NULL,
	UNIQUE(sequence, timestamp)
);
