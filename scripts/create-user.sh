# run this script in root mode

# create a user and a group for heng
# name:     heng
# uid:      1025
# shell:    bash
# home:     /home/heng

set -e

groupadd                        \
    -g 1025                     \
    heng

useradd                         \
    -c "the heng judge system"  \
    -d /home/heng               \
    -m                          \
    -s /bin/bash                \
    -u 1025                     \
    -g 1025                     \
    heng

passwd -l heng
