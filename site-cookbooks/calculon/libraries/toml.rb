def to_toml(config_hash)
  require 'toml-rb'
  # TOML::Generator.new(config_hash).body
  TomlRB.dump(config_hash)
end
