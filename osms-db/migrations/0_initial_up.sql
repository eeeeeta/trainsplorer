-- Initial import of database types from the old horrible non-migration-based
-- way of doing things

-- extensions
CREATE EXTENSION postgis;
CREATE EXTENSION pg_trgm;

-- types
CREATE TYPE "Days" AS (
	mon BOOL,
	tue BOOL,
	wed BOOL,
	thu BOOL,
	fri BOOL,
	sat BOOL,
	sun BOOL
);
CREATE TYPE "StpIndicator" AS ENUM (
	'Cancellation',
	'NewSchedule',
	'Overlay',
	'Permanent',
	'None'
);

-- functions
CREATE FUNCTION days_value_for_iso_weekday(days "Days", wd int)
    RETURNS boolean AS $$
DECLARE
    ret boolean := false;
BEGIN
    CASE wd
        WHEN 1 THEN
            ret := days.mon;
        WHEN 2 THEN
            ret := days.tue;
        WHEN 3 THEN
            ret := days.wed;
        WHEN 4 THEN
            ret := days.thu;
        WHEN 5 THEN
            ret := days.fri;
        WHEN 6 THEN
            ret := days.sat;
        WHEN 7 THEN
            ret := days.sun;
        ELSE
            RAISE EXCEPTION 'must provide a valid ISO weekday';
        END CASE;
        RETURN ret;
END;
$$ LANGUAGE plpgsql;

-- tables (OSM)

CREATE TABLE crossings (
	id SERIAL PRIMARY KEY,
	name VARCHAR,
	area geometry NOT NULL
);

CREATE TABLE nodes (
	id BIGSERIAL PRIMARY KEY,
	location geometry NOT NULL,
	graph_part INT NOT NULL DEFAULT 0,
	parent_crossing INT REFERENCES crossings ON DELETE RESTRICT,
	orig_osm_id BIGINT,
	osm_was_crossing BOOL NOT NULL DEFAULT false
);
CREATE INDEX nodes_id ON nodes (id);
-- XXX: wat, why do we have two indexes?
-- I think nodes_location is redundant
CREATE INDEX nodes_location ON nodes (location);
CREATE INDEX nodes_geom ON nodes USING GIST(location);
CREATE INDEX nodes_orig_osm_id ON nodes (orig_osm_id);

CREATE TABLE links (
	p1 BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
	p2 BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
	way geometry NOT NULL,
	distance REAL NOT NULL,
	UNIQUE(p1, p2)
);

CREATE INDEX links_p1 ON links (p1);
CREATE INDEX links_p2 ON links (p2);
CREATE INDEX links_geom ON links USING GIST (way);

-- XXX: a lot of these station tables need indexes, esp. on nr_ref
-- something to fix in migration #1, methinks
CREATE TABLE stations (
	id SERIAL PRIMARY KEY,
	nr_ref VARCHAR NOT NULL,
	point BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
	area geometry NOT NULL
);

CREATE TABLE station_overrides (
	id SERIAL PRIMARY KEY,
	nr_ref VARCHAR NOT NULL,
	area geometry NOT NULL
);

CREATE TABLE station_navigation_problems (
	id SERIAL PRIMARY KEY,
	geo_generation INT NOT NULL,
	origin INT NOT NULL REFERENCES stations ON DELETE CASCADE,
	destination INT NOT NULL REFERENCES stations ON DELETE CASCADE,
	descrip VARCHAR NOT NULL,
	UNIQUE(geo_generation, origin, destination)
);

CREATE TABLE problematic_stations (
	id SERIAL PRIMARY KEY,
	nr_ref VARCHAR NOT NULL,
	area geometry NOT NULL,
	defect INT NOT NULL
);

CREATE TABLE station_paths (
	s1 INT NOT NULL REFERENCES stations ON DELETE RESTRICT,
	s2 INT NOT NULL REFERENCES stations ON DELETE RESTRICT,
	way geometry NOT NULL,
	nodes BIGINT[] NOT NULL,
	crossings INT[] NOT NULL,
	crossing_locations DOUBLE PRECISION[] NOT NULL,
	id SERIAL PRIMARY KEY,
	UNIQUE(s1, s2),
	CHECK(cardinality(crossings) = cardinality(crossing_locations))
);


-- tables (NTROD)

CREATE TABLE schedules (
	id SERIAL PRIMARY KEY,
	uid VARCHAR NOT NULL,
	start_date DATE NOT NULL,
	end_date DATE NOT NULL,
	days "Days" NOT NULL,
	stp_indicator "StpIndicator" NOT NULL,
	signalling_id VARCHAR,
	geo_generation INT NOT NULL DEFAULT 0,
	source INT NOT NULL DEFAULT 0,
	file_metaseq INT,
	UNIQUE(uid, start_date, stp_indicator, source)
	-- (indexes implicit for primary key and UNIQUE constraints)
);

CREATE TABLE schedule_movements (
	id SERIAL PRIMARY KEY,
	parent_sched INT NOT NULL REFERENCES schedules ON DELETE CASCADE,
	tiploc VARCHAR NOT NULL,
	action INT NOT NULL,
	origterm BOOL NOT NULL,
	time TIME NOT NULL,
	starts_path INT REFERENCES station_paths ON DELETE RESTRICT,
	ends_path INT REFERENCES station_paths ON DELETE RESTRICT
);

CREATE INDEX schedule_movements_parent_sched ON schedule_movements (parent_sched);
CREATE INDEX schedule_movements_tiploc ON schedule_movements (tiploc);
CREATE INDEX schedule_movements_time ON schedule_movements (time);
CREATE INDEX schedule_movements_tiploc_time ON schedule_movements (tiploc, time);

CREATE TABLE trains (
	id SERIAL PRIMARY KEY,
	parent_sched INT NOT NULL REFERENCES schedules ON DELETE CASCADE,
	trust_id VARCHAR NOT NULL,
	date DATE NOT NULL,
	signalling_id VARCHAR NOT NULL,
	cancelled BOOL NOT NULL DEFAULT false,
	terminated BOOL NOT NULL DEFAULT false,
	UNIQUE(trust_id, date)
);

CREATE INDEX trains_parent_sched ON trains (parent_sched);
CREATE INDEX trains_date ON trains (date);
CREATE INDEX trains_trust_id_date ON trains (trust_id, date);

CREATE TABLE train_movements (
	id SERIAL PRIMARY KEY,
	parent_train INT NOT NULL REFERENCES trains ON DELETE CASCADE,
	parent_mvt INT NOT NULL REFERENCES schedule_movements ON DELETE CASCADE,
	time TIME NOT NULL,
	source INT NOT NULL
);

CREATE INDEX train_movements_parent_mvt_parent_train ON train_movements (parent_mvt, parent_train);

CREATE TABLE msn_entries (
	tiploc VARCHAR NOT NULL,
	name VARCHAR NOT NULL,
	cate INT NOT NULL,
	crs VARCHAR NOT NULL
);

CREATE INDEX msn_entries_tiploc ON msn_entries (tiploc);
CREATE INDEX msn_entries_tiploc_trgm ON msn_entries USING gin (tiploc gin_trgm_ops);
CREATE INDEX msn_entries_name_trgm ON msn_entries USING gin (name gin_trgm_ops);
CREATE INDEX msn_entries_crs_trgm ON msn_entries USING gin (crs gin_trgm_ops);

CREATE TABLE naptan_entries (
	atco VARCHAR UNIQUE NOT NULL,
	tiploc VARCHAR PRIMARY KEY,
	crs VARCHAR NOT NULL,
	name VARCHAR NOT NULL,
	loc geometry NOT NULL
);

CREATE TABLE schedule_files (
	id SERIAL PRIMARY KEY,
	timestamp BIGINT UNIQUE NOT NULL,
	metatype VARCHAR NOT NULL,
	metaseq INT NOT NULL
);

-- NB: If you change this, change the brittle SELECT
-- in the station_suggestions() function in osms-web!!
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
CREATE INDEX corpus_entries_nlcdesc_trgm ON corpus_entries USING gin (nlcdesc gin_trgm_ops);
CREATE INDEX corpus_entries_tiploc_trgm ON corpus_entries USING gin (tiploc gin_trgm_ops);
CREATE INDEX corpus_entries_crs_trgm ON corpus_entries USING gin (crs gin_trgm_ops);
