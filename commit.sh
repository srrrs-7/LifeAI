#!/bin/bash

msg=$1

git add .
git commit -m "to main: $msg"
git push origin main