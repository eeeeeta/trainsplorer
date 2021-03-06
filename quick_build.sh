#!/bin/bash
echo "CPU type: $1, branch name: $2"
echo "This is going to be (reasonably) expensive."
gcloud builds submit --config=cloudbuild.yaml "--machine-type=n1-highcpu-$1" --substitutions "COMMIT_SHA=$(git rev-parse HEAD),BRANCH_NAME=$2" .
