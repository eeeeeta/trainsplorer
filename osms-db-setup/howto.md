# How to setup osm-signal

## Initial database setup

- Clone this repo.
- Install postgres & postgis: `pacman -S postgresql postgis`
  - Configuring PostgreSQL is outside the scope of this tutorial.
- Make a new database called 'osm': `createdb osm`
- Enable the 'postgis' extension:
  ```
  $ psql -d osm
  osm=# CREATE EXTENSION postgis;
  CREATE EXTENSION
  osm=# \q
  ```
- Download greater-london-latest.osm.pbf
  from
  [Geofabrik downloads](http://download.geofabrik.de/europe/great-britain/england/greater-london.html)
- Install osm2pgsql: `pacaur -S osm2pgsql-git`
- Run osm2pgsql: `osm2pgsql -s -l -d osm greater-london-latest.osm.pbf`
  - If running on a device with low memory, try `--cache 100M`.
  - If running on an ARM device, you may get alignment faults. If this happens,
    consider using another machine to do the import, and connecting to the
    database remotely.
- For the next parts, you will need a Network Rail Open Data account, and be
  subscribed to the SCHEDULE and Reference Data feeds on it.
  - For more information, including how to get an account,
    go [here](http://nrodwiki.rockshore.net/index.php/About_the_feeds)
- Download `CIF_ALL_FULL_DAILY.json`
  from
  [here](https://datafeeds.networkrail.co.uk/ntrod/CifFileAuthenticate?type=CIF_ALL_FULL_DAILY&day=toc-full)
  - You will have to authenticate with your NTROD credentials for both of the downloads.
  - You may need to rename it to have the extension `.json.gz` and `gunzip` it.
- Download `CORPUSExtract.json`
  from
  [here](http://datafeeds.networkrail.co.uk/ntrod/SupportingFileAuthenticate?type=CORPUS)
  - You may need to rename it to have the extension `.json.gz` and `gunzip` it.
- Compile the setup utility, `osms-db-setup`, with `cargo build`.
- Make a `config.toml` like this:
  ```toml
  database_url = "postgresql://USER@ADDRESS/DATABASE_NAME"
  corpus_data = "/path/to/CORPUSExtract.json"
  schedule_data = "/path/to/CIF_ALL_FULL_DAILY.json"
  # Optionally, to limit the train companies used in the schedule data:
  # limit_schedule_toc = "SW" (or any other train operating company ATOC code)
  ```
  - (A list of ATOC codes can be found [here](http://nrodwiki.rockshore.net/index.php/TOC_Codes))
- Run the tool with `cargo run`. Make yourself a cup of tea, then repeat for a
  few hours until it's done. Watch the soothing progress messages scroll by.
