#!/bin/bash
echo "CPU type: $1"
echo "This is going to be (reasonably) expensive."
gcloud builds submit --config=cloudbuild.yaml "--machine-type=n1-highcpu-$1" --substitutions "COMMIT_SHA=$(git rev-parse HEAD)" --subsitutions "BRANCH_NAME=quickbuild" .
