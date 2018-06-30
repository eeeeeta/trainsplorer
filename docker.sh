#!/bin/bash

docker build -t osms-nrod --target osms-nrod .
docker build -t osms-web --target osms-web .
docker build -t osms-db-setup --target osms-db-setup .
