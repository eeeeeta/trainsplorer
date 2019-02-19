the trainsplorer (aka osm-signal)
=================================

[![Build Status](https://travis-ci.org/eeeeeta/osm-signal.svg?branch=master)](https://travis-ci.org/eeeeeta/osm-signal)
[![IRC chat @ #trainsplorer on freenode](https://img.shields.io/badge/irc-%23trainsplorer%20on%20freenode-blue.svg)](https://kiwiirc.com/client/chat.freenode.net/#trainsplorer)
![GNU AGPLv3 licensed](https://www.gnu.org/graphics/agplv3-155x51.png)

## What is this?

This is an in-development project that processes Network Rail's [Open Rail Data](https://wiki.openraildata.com/index.php/Main_Page)
feeds - absorbing all sorts of information about train schedules, and keeping a record of current and historic live train data.
The eventual end goal of the project is to link this information into OpenStreetMap geodata, in order to provide cool things like
a live map of train locations (approximately, that is), predictions on level crossing opening & closing times, and the ability
to find out which trains are going to pass any location on the railway at any given time. It's all written in
[Rust](https://www.rust-lang.org/en-US/) as well, because Rust is cool.

However, it's very much not done yet! Watch this space for further stuff! Also, feel free to join the [chatroom](https://kiwiirc.com/client/chat.freenode.net/#trainsplorer)
(#trainsplorer on chat.freenode.net), if you'd like to discuss the project (or anything about trains in general, really)

## What are all these moving parts?

This repository contains many individual Rust crates. Here's a short overview of what they do:

- `atoc-msn`: parses the Master Station Names (MSN) file from the [Rail Delivery Group](http://data.atoc.org)'s Industry Data dataset
- `ntrod-types`: parses data from the [Network Rail open data feeds](https://wiki.openraildata.com/index.php/About_the_Network_Rail_feeds),
  specifically the SCHEDULE, Train Movements, reference data, and VSTP feeds
- `osms-db`: the main database library for the project; handles storing data into and retrieving data from a PostgreSQL database, as
  well as performing some other handy utility functions, such as navigating between two points on the railway
- `osms-db-setup`: builds on `osms-db`, and contains a utility for loading data into the database in the first place
- `osms-nrod`: connects to the Network Rail STOMP messaging service and the National Rail Enquiries Darwin feed, and processes real-time train data, storing it in the database
  using `osms-db`
- `osms-darwin`: used for testing the Darwin feed
- `osms-web`: a fancy webserver with lots of buttons to press that enables people to admire the wonderful collections of data in the
  database
- `doc`: that's not a crate, that's a directory containing mostly incoherent design notes and the like

## Can I have some screenshots?

Yes! That's what `osms-web` is for, after all. Here you go:

!['search for trains' interface'](https://i.imgur.com/TKPlSq9.png)
![movement search interface](https://i.imgur.com/2c92BfZ.png)
![train details](https://i.imgur.com/gOe4wjF.png)
![schedule details](https://i.imgur.com/uf4uPmE.png)

## Licensing

All crates in this repository are free software: you can redistribute them and/or modify
them under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

The `atoc-msn`, `national-rail-departures`, and `ntrod-types` crates are also
licensed under Apache 2.0 and MIT terms. This means that you can
use these crates (but *only* these crates) under AGPLv3, MIT, or Apache 2.0
at your option.
