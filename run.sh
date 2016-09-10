#!/bin/bash

berks vendor
sudo chef-client -z -c chef-client.rb -o 'role[devmachine]'
