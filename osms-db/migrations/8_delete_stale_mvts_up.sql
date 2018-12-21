-- Delete useless estimations: these were created by prior versions of osms-nrod, which failed to clear them out after a train was cancelled or terminated.
DELETE FROM train_movements WHERE estimated = true AND EXISTS(SELECT * FROM trains WHERE trains.id = train_movements.parent_train AND trains.cancelled = true);
DELETE FROM train_movements WHERE estimated = true AND EXISTS(SELECT * FROM trains WHERE trains.id = train_movements.parent_train AND trains.terminated = true);
