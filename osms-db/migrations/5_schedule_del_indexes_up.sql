-- Adds indexes on foreign keys needed to delete schedules efficiently.

CREATE INDEX train_movements_parent_mvt ON train_movements (parent_mvt);
CREATE INDEX train_movements_parent_train ON train_movements (parent_train);
