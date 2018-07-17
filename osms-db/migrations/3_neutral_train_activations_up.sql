-- Makes trains activatable from either TRUST or Darwin.

ALTER TABLE trains ADD CONSTRAINT trains_parent_sched_date_unique UNIQUE(parent_sched, date);
ALTER TABLE trains ALTER COLUMN trust_id DROP NOT NULL;
ALTER TABLE trains ALTER COLUMN signalling_id DROP NOT NULL;
