## Note
This is a mirror of https://gitlab.com/snow-star/tap-demo

# Description
[![pipeline status](https://gitlab.com/snow-star/tap-demo/badges/master/pipeline.svg)](https://gitlab.com/snow-star/tap-demo/commits/master)

tap tunnel via udp demo, this demo creates a __virtual tap network interface__, and send packets from tap devices to other nodes and vice versa.

This project was originally created in cloud computing course in NCU.

More details in docker hub: https://hub.docker.com/u/snowstar/tap-demo

## Features
- [x] auto discovery other nodes using multicast
- [ ] encrypt
- [ ] auto assign ip address
- [x] use IPC to control nodes.

# How to use this image

## Requirements
- kernel tuntap support

## (Optional) Create docker network
```
docker network create tap-tunnel
```

## Start 
### Option 1: scan manually
#### Start service
```bash
# start peer 1
docker run --name peer-1 --rm --cap-add=NET_ADMIN --device /dev/net/tun:/dev/net/tun --network tap-tunnel snowstar/tap-demo start
# start peer 2
docker run --name peer-2 --rm --cap-add=NET_ADMIN --device /dev/net/tun:/dev/net/tun --network tap-tunnel snowstar/tap-demo start
# or start more
```
#### Scan node
```bash
docker exec peer-1 peers scan
docker exec peer-2 peers scan
```

### Option 2: auto discovery
```bash
# start peer 1
docker run --name peer-1 --rm --cap-add=NET_ADMIN --device /dev/net/tun:/dev/net/tun --network tap-tunnel snowstar/tap-demo start -a
# start peer 2
docker run --name peer-2 --rm --cap-add=NET_ADMIN --device /dev/net/tun:/dev/net/tun --network tap-tunnel snowstar/tap-demo start -a
# or start more
```

### Option 3: add peers manually later
#### Start service
```bash
# start peer 1
docker run --name peer-1 --rm --cap-add=NET_ADMIN --device /dev/net/tun:/dev/net/tun --network tap-tunnel snowstar/tap-demo start
# start peer 2
docker run --name peer-2 --rm --cap-add=NET_ADMIN --device /dev/net/tun:/dev/net/tun --network tap-tunnel snowstar/tap-demo start
# or start more
```

#### Add peers
```bash
docker exec peer-1 peers add peer-2 peer-2:9909
docker exec peer-2 peers add peer-1 peer-1:9909
```

## Assign IP
```bash
docker exec peer-1 ip a add 10.0.0.1/24 dev tap0
docker exec peer-1 ip l set mtu 1460 tap0

docker exec peer-2 ip a add 10.0.0.2/24 dev tap0
docker exec peer-2 ip l set mtu 1460 tap0
```

## Test
__Notice: It could take 1 or 2 minutes to let every node discovery and establish connection with each other if using auto mode__
```bash
docker exec peer-1 ping 10.0.0.2

docker exec peer-2 ping 10.0.0.1
```

# Benchmark
__Hardware: i7-6700 HQ, 8 G RAM, Intel 545s 512G SSD__

__Software: Arch Linux, Docker 18.09.6-ce, Nginx 1.14.0__

Download directly  
![Direct](https://gitlab.com/snow-star/tap-demo/raw/master/assets/benchmark-01.png)

Download via tunnel  
![Via tunnel](https://gitlab.com/snow-star/tap-demo/raw/master/assets/benchmark-02.png)
