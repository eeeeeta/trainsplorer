CREATE INDEX train_movements_parent_mvt ON train_movements (parent_mvt);
CREATE INDEX train_movements_parent_train ON train_movements (parent_train);
CREATE INDEX schedule_movements_time ON schedule_movements (time);
CREATE INDEX schedule_movements_tiploc ON schedule_movements (tiploc);
