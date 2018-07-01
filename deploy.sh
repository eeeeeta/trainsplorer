#!/bin/bash

docker tag osms-nrod gcr.io/osm-signal/osms-nrod:initial
docker push gcr.io/osm-signal/osms-nrod:initial
docker tag osms-db-setup gcr.io/osm-signal/osms-db-setup:initial
docker push gcr.io/osm-signal/osms-db-setup:initial

