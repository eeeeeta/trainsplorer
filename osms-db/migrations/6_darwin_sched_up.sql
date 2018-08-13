ALTER TABLE schedules ADD COLUMN darwin_id VARCHAR UNIQUE;
ALTER TABLE trains ADD COLUMN parent_nre_sched INT REFERENCES schedules;
CREATE INDEX trains_parent_nre_sched ON trains (parent_nre_sched);
