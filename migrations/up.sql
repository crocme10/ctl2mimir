create table if not exists indexes (
  index_id integer not null primary key autoincrement,
  index_type text not null,
  data_source text not null,
  region text not null,
  status text default '{"type": "NotAvailable"}',
  created_at integer not null default (strftime('%s', 'now')),
  updated_at integer not null default (strftime('%s', 'now'))
);

