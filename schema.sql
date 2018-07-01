create table issue (
  filepath TEXT PRIMARY KEY,
  modified_at TEXT NOT NULL,
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

create index issue_modified_at on issue(modified_at);
create index issue_publisher_series on issue(publisher, series);
create index issue_released_at on issue(released_at);

create virtual table issue_fts USING FTS4(issue_id, comicinfo);
