ALTER TABLE schedule_movements ADD COLUMN idx INT;
ALTER TABLE schedule_movements ADD CONSTRAINT schedule_movements_unique_idx UNIQUE (parent_sched, idx); 
