-- First cut of Darwin support. More to follow!

ALTER TABLE trains ADD COLUMN nre_id VARCHAR;
ALTER TABLE train_movements ADD COLUMN estimated BOOL NOT NULL DEFAULT false;

CREATE INDEX trains_nre_id ON trains (nre_id);

CREATE TABLE movement_sources (
	id INT PRIMARY KEY,
	source_type INT NOT NULL,
	source_text VARCHAR NOT NULL
);

CREATE INDEX movement_sources_source_type_source_text ON movement_sources (source_type, source_text);

INSERT INTO movement_sources (id, source_type, source_text) VALUES (0, 0, 'TRUST Train Movements');
INSERT INTO movement_sources (id, source_type, source_text) VALUES (1, 1, 'Darwin Push Port (generic)');

ALTER TABLE train_movements ADD CONSTRAINT train_movements_source_fkey FOREIGN KEY (source) REFERENCES movement_sources (id) ON DELETE RESTRICT;
