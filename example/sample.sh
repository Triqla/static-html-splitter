#!/bin/bash
rm -rf origin/*
unzip $@ -d origin
rm -rf autogenerated_components/*
static-html-splitter origin/index.html autogenerated_components react.toml
cp origin/* public -r
prettier next-client/autogenerated_components/**.tsx --write
