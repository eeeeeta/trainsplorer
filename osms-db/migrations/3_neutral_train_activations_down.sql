ALTER TABLE trains ALTER COLUMN signalling_id SET NOT NULL;
ALTER TABLE trains ALTER COLUMN trust_id SET NOT NULL;
ALTER TABLE trains DROP CONSTRAINT IF EXISTS trains_parent_sched_date_unique;
