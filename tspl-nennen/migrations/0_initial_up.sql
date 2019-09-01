CREATE TABLE corpus_entries (
	stanox VARCHAR,
	uic VARCHAR,
	crs VARCHAR,
	tiploc VARCHAR,
	nlc VARCHAR,
	nlcdesc VARCHAR,
	nlcdesc16 VARCHAR
);

CREATE INDEX corpus_entries_stanox ON corpus_entries (stanox);
CREATE INDEX corpus_entries_tiploc ON corpus_entries (tiploc);
CREATE INDEX corpus_entries_crs ON corpus_entries (crs);

CREATE TABLE msn_entries (
	name VARCHAR NOT NULL,
	cate_type INT NOT NULL,
	tiploc VARCHAR NOT NULL,
	subsidiary_crs VARCHAR NOT NULL,
	crs VARCHAR NOT NULL,
	easting INT NOT NULL,
	estimated BOOL NOT NULL,
	northing INT NOT NULL,
	change_time INT NOT NULL
);

CREATE INDEX msn_entries_crs ON msn_entries (crs);
CREATE INDEX msn_entries_tiploc ON msn_entries (tiploc);

CREATE TABLE station_names (
	id INTEGER PRIMARY KEY,
	name VARCHAR NOT NULL,
	tiploc VARCHAR UNIQUE,
	crs VARCHAR UNIQUE,
	CHECK((tiploc IS NULL) != (crs IS NULL))
);

CREATE VIRTUAL TABLE station_names_idx USING fts5(name, tiploc, crs, content='station_names', content_rowid='id');

-- Triggers to keep the FTS index up to date.
-- Copied from https://www.sqlite.org/fts5.html § 4.4.2

CREATE TRIGGER sn_ai AFTER INSERT ON station_names BEGIN
  INSERT INTO station_names_idx(rowid, name, tiploc, crs) VALUES (new.id, new.name, new.tiploc, new.crs);
END;
CREATE TRIGGER sn_ad AFTER DELETE ON station_names BEGIN
  INSERT INTO station_names_idx(station_names_idx, rowid, name, tiploc, crs) VALUES('delete', old.id, old.name, old.tiploc, old.crs);
END;
CREATE TRIGGER sn_au AFTER UPDATE ON station_names BEGIN
  INSERT INTO station_names_idx(station_names_idx, rowid, name, tiploc, crs) VALUES('delete', old.id, old.name, old.tiploc, old.crs);
  INSERT INTO station_names_idx(rowid, name, tiploc, crs) VALUES (new.id, new.name, new.tiploc, new.crs);
END;

