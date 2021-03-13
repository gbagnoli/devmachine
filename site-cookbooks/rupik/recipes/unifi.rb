# seems like builds for arm64 are only for mongo 4.4
# but unifi wants mongo >2.0 <4.0
# commenting out

# apt_repository 'unifi' do
#   arch "armhf"
#   uri "https://www.ui.com/downloads/unifi/debian"
#   distribution "stable"
#   components ["ubiquiti"]
#   keyserver "keyserver.ubuntu.com"
#   key "06E85760C0A52C50"
# end

# apt_repository 'mongodb' do
#   arch "arm64"
#   uri "https://repo.mongodb.org/apt/ubuntu"
#   distribution "bionic/mongodb-org/4.4"
#   components ["multiverse"]
#   key "https://www.mongodb.org/static/pgp/server-4.4.asc"
# end

# package "unifi"

# service "unifi" do
#   action %i[enable start]
# end
