#!/bin/bash

while true; do
    echo -n "$(date): ";
    echo "$(( $(cat /sys/class/thermal/thermal_zone0/temp) / 1000 ))°C";
    sleep 1;
done
