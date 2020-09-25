# ctl2mimir

Declarative interface for driving Elasticsearch index management. It is designed to hide all the
machinery necessary to create Elasticsearch index. A user would just need to use ctl2mimir for
creating indices, and bragi to query them.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Getting Started

These instructions will get you a copy of the project up and running on your local machine for
development and testing purposes. See deployment for notes on how to deploy the project on a live
system.

### Prerequisites

`ctl2mimir` relies on [0MQ](https://zeromq.org) and [SQLite](https://sqlite.org)

For debian environments, you would need to install the following packages:

```
apt-get update && apt-get install sqlite3 && apt-get install libzmq5-dev
```

It is also assumed that you have [rust](https://www.rust-lang.org) installed on your machine.
You can find instructions [here](https://www.rust-lang.org/tools/install).

### Installing

This is a straightforward rust project, so a dollop of `cargo` should do the trick:

```
git clone https://github.com/riendegris/ctl2mimir
cd ctl2mimir
cargo build --release
```

This will create a binary in `target/release/server`

Alternatively, you can construct a docker container

```
git clone https://github.com/riendegris/ctl2mimir
cd ctl2mimir
docker build -t ctl2mimir -f ./docker/Dockerfile .
```

## Running

Start an elasticsearch (eg using docker) and then the server:

```
docker run -d --name es -p 9200:9200 elasticsearch:2
sqlite3 hello.db < indexes.sql
ES_CONN_STR=http://elasticsearch:9200 ./target/release/server -h 0.0.0.0 -u sqlite://hello.db
```

## Running the tests

Explain how to run the automated tests for this system. Err, what, I have to write tests ???
Sorry, didn't get the memo.

### Break down into end to end tests

```
cargo test --release
```

## Deployment

Add additional notes about how to deploy this on a live system

## Built With

These are some of the crates used:

* [juniper](https://docs.rs/juniper/0.14.2/juniper/) - Graphql implementation in rust
* [warp](https://docs.rs/warp/0.2.3/warp/) - Web framework
* [sqlx](https://docs.rs/sqlx/0.3.5/sqlx/) - SQLite interface
* [0MQ](ihttps://docs.rs/async_zmq/0.3.2/async_zmq/) - PubSub

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process
for submitting pull requests to us.

## Versioning

We use [SemVer](http://semver.org/) for versioning. For the versions available, see the [tags on
this repository](https://github.com/your/project/tags). 

## Authors

* **Matthieu Paindavoine** - *Initial work* - [riendegris](https://github.com/riendegris)

See also the list of [contributors](https://github.com/riendegris/ctl2mimir/contributors) who
participated in this project.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details

## Acknowledgments

Coming up
