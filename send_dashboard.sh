#!/bin/bash
unset SSH_AUTH_SOCK
scp -o StrictHostKeyChecking=no -o PasswordAuthentication=no -i ~/.ssh/id_rsa ~/code/headroom-kindle/dashboard.png root@192.168.15.244:/mnt/us/dashboard.png

# Siccome la versione di fbink è compilata senza supporto immagini (!), usiamo eips, killando la UI prima per essere certi.
ssh -o StrictHostKeyChecking=no -o PasswordAuthentication=no -i ~/.ssh/id_rsa root@192.168.15.244 'stop lab126_gui 2>/dev/null; /usr/sbin/eips -c; sleep 1; /usr/sbin/eips -f -g /mnt/us/dashboard.png'
