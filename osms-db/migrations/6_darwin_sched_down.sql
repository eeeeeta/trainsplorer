DROP INDEX trains_parent_nre_sched;
ALTER TABLE trains DROP COLUMN parent_nre_sched;
ALTER TABLE schedules DROP COLUMN darwin_id;
