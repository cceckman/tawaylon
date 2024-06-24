#!/bin/sh

# Update protocol files from upstream wlroots.

curl -Lo wlroots-protocols.tar.gz \
	https://gitlab.freedesktop.org/wlroots/wlroots/-/archive/master/wlroots-master.tar.gz?path=protocol

tar -xvf wlroots-protocols.tar.gz --strip-components=1 --wildcards '*.xml'

