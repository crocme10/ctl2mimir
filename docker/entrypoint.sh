#!/bin/sh

echo "Waiting for elasticsearch..."

while ! nc -z elasticsearch 9200; do
  sleep 0.1
done

echo "Elasticsearch started"
movine init
./service init
./service run
