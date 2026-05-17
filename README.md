# What if a Town Square was just a Town Square?

This project is part of the greater PlaceNet project. 

The goal of PlaceNet is to give in-person communities better means for communicating with each other while encouraging healthy user engagement. We hope that PlaceNet websites will be able to serve as digital third spaces that foster connections between people in the same area without the need for a large governing body or algorithmic control. Unlike other solutions to this problem, PlaceNet is not an App or Content Platform, it is instead infrastructure for other apps and platforms to build on top of. PlaceNet is less like NextDoor and more like the World Wide Web.


## What is PlaceNet?

PlaceNet is a networking infrastructure built on top of a Proof of Presence (PoP) protocol for creating secure Peer to Peer (P2P) connections with other services within a physical area. This is achieved by broadcasting rotating encryption keys over Long Range Radio (LoRa) for both discovery and authentication. Upon verifying that another node is within broadcast distance (~600m - ~2km) using PoP, the nodes will establish a P2P connection via WireGuard with the help of a Coordination server accessible over the open internet. Once connected, PlaceNet nodes are able to share content between themselves as if they were on the same network.

## What is The Cloud Gateway?

The Cloud Gateway serves as a STUN server and is the main coordination service for establishing connections between content servers. Since most content servers (Hamlets) are not accessible over the open internet a process called NAT traversal must be completed to establish a P2P connection. The gateway will also serve as a way to establish unique identifiers for Hamlets, to minimize the risk of malicious nodes and remove the need for complicated naming conventions.

For an in depth explanation of NAT traversal, Tailscale has a great article on the subject: https://tailscale.com/blog/how-nat-traversal-works

Coordination servers operate on zero trust principles. They do not have the ability to see any content running through them. Additionally Coordination servers will operate on a federated architecture inspired by the Fediverse/ActivityPub protocol - meaning that PlaceNet does not rely on a centralized coordination service and instead can use many other providers. 

## Current state of the project

The cloud gateway is in a highly developmental state. Consider all code in this repository subject to change. The gateway is currently used entirely to test the beacon and Hamlet projects.

PlaceNet as a whole is currently in early Alpha. PlaceNet nodes are able to send messages to each other over the internet but are unable to serve content or route through other services. NAT traversal is currently in the works. If you would like to support this project please consider making a contribution to the following projects:


[PlaceNet Beacon](https://github.com/marcus-wrrn/PlaceNet-Beacon)
- Device responsible for broadcasting PlaceNet messages via LoRa

[PlaceNet Hamlet](https://github.com/marcus-wrrn/PlaceNet-Hamlet)
- Main service for serving PlaceNet web content

## AI Usage

AI is used throughout this project to allow for faster development time and as a means of learning. However all code is consistently verified, refactored and planned out. Due to the nature of the project, it is not enough to have a working prototype, the networking logic must also be understood and documented. It is the full intent of the project that others will be able to write their own implementations for various parts of PlaceNet.   

Vibe coding is greatly discouraged when making contributions.

## Similar Projects

PlaceNet is greatly inspired by both Meshtastic and Tailscale. While PlaceNet differs in many key ways, understanding both projects can help one understand how PlaceNet aims to be implemented.

[Meshatastic](https://meshtastic.org/)
[Tailscale](https://tailscale.com/)
