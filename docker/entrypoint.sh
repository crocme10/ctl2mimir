#!/bin/sh

echo "Waiting for elasticsearch..."

while ! nc -z elasticsearch 9200; do
  sleep 0.1
done

echo "Elasticsearch started"
echo "Settings: ${SETTINGS}"
echo "Database Name: ${DATABASE_NAME}"
export RUST_LOG=debug
export DATABASE_URL=sqlite:///var/opt/mimir_ingest/${DATABASE_NAME}
./service init -c /etc/opt/mimir_ingest -s ${SETTINGS}
# This sleep is necessary otherwise the database is locked when we try to access it
# when the service is running.
sleep 1
./service run -c /etc/opt/mimir_ingest -s ${SETTINGS}
