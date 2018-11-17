# shamelessly stolen from openssl cookbook
#

module Marvin
  # random password generator
  module RandomPassword
    def self.included(_base)
      require 'securerandom' unless defined?(SecureRandom)
    end

    def random_password(options = {})
      length = options[:length] || 24
      SecureRandom.hex(length)
    end
  end
end
