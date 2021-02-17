set -e
controllers="cpu memory pids"
for controller in $controllers
do
    echo /sys/fs/cgroup/$controller/heng-sandbox
    cd /sys/fs/cgroup/$controller/heng-sandbox
    ls -d */ | xargs rmdir
done
