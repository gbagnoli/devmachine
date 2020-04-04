# frozen_string_literal: true

#
# Cookbook Name:: ubik
# Recipe:: default
#
# The MIT License (MIT)
#
# Copyright (c) 2016 Giacomo Bagnoli
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in
# all copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
# THE SOFTWARE.

include_recipe "ubik::ubuntu_hwe"
include_recipe "ubik::packages"
include_recipe "ubik::langs"
include_recipe "ubik::latex" if node["ubik"]["install_latex"]
include_recipe "ubik::golang"
include_recipe "ubik::ruby"
include_recipe "ubik::mtrack" if node["ubik"]["enable_mtrack"]
include_recipe "ubik::printer"
include_recipe "ubik::fonts" if node["ubik"]["install_fonts"]

node.override["dnscrypt_proxy"]["listen_address"] = "127.0.2.1"
include_recipe "dnscrypt_proxy"
