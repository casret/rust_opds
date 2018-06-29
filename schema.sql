create table issue (
  filepath TEXT PRIMARY KEY,
  imported_at TEXT NOT NULL,
  read_at TEXT,
  comicvine_id INTEGER,
  comicvine_url TEXT,
  series TEXT,
  issue_number INTEGER,
  volume INTEGER,
  title TEXT,
  summary TEXT,
  released_at TEXT,
  writer TEXT,
  penciller TEXT,
  inker TEXT,
  colorist TEXT,
  cover_artist TEXT,
  publisher TEXT,
  page_count INTEGER
);

create index issue_imported_at on issue(imported_at);
create index issue_read_at on issue(read_at);
create index issue_publisher_series on issue(publisher, series);
create index issue_released_at on issue(released_at);

create virtual table issue_fts USING FTS4(comicinfo);
