ALTER TABLE schedule_movements DROP CONSTRAINT schedule_movements_ends_path_fkey;
ALTER TABLE schedule_movements DROP CONSTRAINT schedule_movements_starts_path_fkey;
UPDATE schedule_movements SET starts_path = NULL, ends_path = NULL;
UPDATE schedules SET geo_generation = 0;
DROP TABLE problematic_stations;
DROP TABLE station_overrides;
DROP TABLE station_paths;
DROP TABLE station_navigation_problems;
DROP TABLE stations;
CREATE TABLE railway_locations (
	id SERIAL PRIMARY KEY,
	name VARCHAR NOT NULL,
	point BIGINT NOT NULL REFERENCES nodes ON DELETE CASCADE,
	area geometry NOT NULL,
	stanox VARCHAR UNIQUE,
	tiploc VARCHAR[] NOT NULL,
	crs VARCHAR[] NOT NULL,
	defect INT
);
CREATE TABLE station_paths (
	id SERIAL PRIMARY KEY,
	s1 INT NOT NULL REFERENCES railway_locations ON DELETE RESTRICT,
	s2 INT NOT NULL REFERENCES railway_locations ON DELETE RESTRICT,
	way geometry NOT NULL,
	nodes BIGINT[] NOT NULL,
	UNIQUE(s1, s2)
);
ALTER TABLE schedule_movements ADD CONSTRAINT schedule_movements_starts_path_fkey FOREIGN KEY (starts_path) REFERENCES station_paths ON DELETE RESTRICT;
ALTER TABLE schedule_movements ADD CONSTRAINT schedule_movements_ends_path_fkey FOREIGN KEY (ends_path) REFERENCES station_paths ON DELETE RESTRICT;
