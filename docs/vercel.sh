#!/bin/bash

# Check if there are changes in the docs directory
git diff --quiet HEAD^ HEAD -- ../docs/
has_docs_changes=$?
echo ">> Docs diff status $has_docs_changes"

# Deploy if:
# 1. It's the master branch (production deployment), OR
# 2. There are changes in the docs directory (preview deployment)
if [[ $VERCEL_GIT_COMMIT_REF == "master" ]] || [ $has_docs_changes == 1 ]; then
  echo ">> Proceeding with deployment."
  exit 1;
else
  echo ">> Skipping deployment (no docs changes detected)."
  exit 0;
fi