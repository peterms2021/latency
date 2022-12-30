# Latency Tracker 

A client & server latency measuring app  using Prometheus Metrics in a Rust Web Service. The echo server can be depployed stand-alone at node in a cluster and the echo clients can be run as side-cars in application pods. The system measures the responsiveness of the ecosystem similar to ping but without the the use of ICMP. More importantly, we can run the client as service that monitors the liveliness of cluster with latency as primary measure. The goal is to be able to measure latency with high precision and high fidelity - nano or microseconds.

Build the llop (see the Makefile)
target/debug/llop 

Build prometheus with:

```bash
cd prom
docker build -t prometheus .
```

Run it with:

```bash
docker run -p 9090:9090 --network=host prometheus
```

Note that it network configuration with promethues in docker may not work because of the NAT mode of the bridge. it may be easier to just run a local prometheus instance to see the test mode at work.
```bash
prometheus --config.file=prom/prometheus.yml
```

Run service with `make dev`.

Go to http://localhost:9090/graph and enter the following commands to see the quantiles of `response_time`:

```bash
histogram_quantile(0.50, sum(rate(response_time_bucket{env="testing"}[2m])) by (le))
histogram_quantile(0.90, sum(rate(response_time_bucket{env="testing"}[2m])) by (le))
histogram_quantile(0.99, sum(rate(response_time_bucket{env="testing"}[2m])) by (le))

```

And the rate of increase for response codes by Status Type:

```bash
sum(increase(response_code{env="testing"}[5m])) by (type)
```

You can also call the `/some` endpoint and observer the `incoming_requests` counter, as well as connect via websockets at `/ws/some_id` and observe the `connected_clients` counter.
