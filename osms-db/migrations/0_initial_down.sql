-- Everything must die!

DROP TABLE IF EXISTS corpus_entries;
DROP TABLE IF EXISTS schedule_files;
DROP TABLE IF EXISTS naptan_entries;
DROP TABLE IF EXISTS msn_entries;
DROP TABLE IF EXISTS train_movements;
DROP TABLE IF EXISTS trains;
DROP TABLE IF EXISTS schedule_movements;
DROP TABLE IF EXISTS schedules;
DROP TABLE IF EXISTS station_paths;
DROP TABLE IF EXISTS problematic_stations;
DROP TABLE IF EXISTS station_navigation_problems;
DROP TABLE IF EXISTS station_overrides;
DROP TABLE IF EXISTS stations;
DROP TABLE IF EXISTS links;
DROP TABLE IF EXISTS nodes;
DROP TABLE IF EXISTS crossings;
DROP FUNCTION IF EXISTS days_value_for_iso_weekday(days "Days", wd int);
DROP TYPE IF EXISTS "StpIndicator";
DROP TYPE IF EXISTS "Days";
DROP EXTENSION IF EXISTS postgis;
DROP EXTENSION IF EXISTS pg_trgm;
