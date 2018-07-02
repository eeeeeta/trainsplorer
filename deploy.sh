#!/bin/bash

docker tag osms-nrod eeeeeta/osm-signal:osms-nrod
docker push eeeeeta/osm-signal:osms-nrod
docker tag osms-db-setup eeeeeta/osm-signal:osms-db-setup
docker push eeeeeta/osm-signal:osms-db-setup

