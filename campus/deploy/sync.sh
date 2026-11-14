#!/bin/sh
# Quellen auf den Server spiegeln (ohne target/pkg). Aufruf: sh campus/deploy/sync.sh
cd "$(dirname "$0")/../.." && tar czf - --exclude=campus/target --exclude=campus/web/pkg campus | ssh -o BatchMode=yes root@62.83.21.224 'mkdir -p /opt && tar xzf - -C /opt --no-same-owner'
