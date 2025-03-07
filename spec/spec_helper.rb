# frozen_string_literal: true

require "wasmtime"

RSpec.shared_context("default lets") do
  let(:engine) { Wasmtime::Engine.new }
  let(:store_data) { Object.new }
  let(:store) { Wasmtime::Store.new(engine, store_data) }
  let(:wat) { "(module)" }

  def compile(wat)
    mod = Wasmtime::Module.new(engine, wat)
    Wasmtime::Instance.new(store, mod)
  end
end

RSpec.shared_context(:tmpdir) do
  let(:tmpdir) { Dir.mktmpdir }

  after(:each) do
    FileUtils.rm_rf(tmpdir)
  rescue Errno::EACCES => e
    warn "WARN: Failed to remove #{tmpdir} (#{e})"
  end
end

module WasmFixtures
  include Wasmtime
  extend self

  def wasi_debug
    @wasi_debug_module ||= Module.from_file(Engine.new, "spec/fixtures/wasi-debug.wasm")
  end
end

RSpec.configure do |config|
  config.filter_run focus: true
  config.run_all_when_everything_filtered = true
  if ENV["CI"]
    config.before(focus: true) { raise "Do not commit focused tests (`fit` or `focus: true`)" }
  end

  config.include_context("default lets")

  # So memcheck steps can still pass if RSpec fails
  config.failure_exit_code = ENV.fetch("RSPEC_FAILURE_EXIT_CODE", 1).to_i
  config.default_formatter = ENV.fetch("RSPEC_FORMATTER") do
    config.files_to_run.one? ? "doc" : "progress"
  end

  # Enable flags like --only-failures and --next-failure
  config.example_status_persistence_file_path = ".rspec_status" unless ENV["CI"]

  # Disable RSpec exposing methods globally on `Module` and `main`
  config.disable_monkey_patching!

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end

  if ENV["GC_STRESS"]
    config.around :each do |ex|
      GC.stress = true
      ex.run
    ensure
      GC.stress = false
    end
  end
end

at_exit { GC.start(full_mark: true) } if ENV["GC_AT_EXIT"] == "1"
