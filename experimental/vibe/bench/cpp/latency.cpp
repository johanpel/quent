/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include <benchmark/benchmark.h>
#include <opentelemetry/exporters/ostream/log_record_exporter.h>
#include <opentelemetry/logs/log_record.h>
#include <opentelemetry/sdk/logs/exporter.h>
#include <opentelemetry/sdk/logs/logger_provider.h>
#include <opentelemetry/sdk/logs/logger_provider_factory.h>
#include <opentelemetry/sdk/logs/read_write_log_record.h>
#include <opentelemetry/sdk/logs/simple_log_record_processor_factory.h>
#include <quill/Backend.h>
#include <quill/Frontend.h>
#include <quill/LogMacros.h>
#include <quill/sinks/JsonSink.h>
#include <quill/sinks/NullSink.h>
#include <spdlog/logger.h>
#include <spdlog/sinks/basic_file_sink.h>
#include <spdlog/sinks/null_sink.h>

#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>

#include "quent-bench-cpp-bridge/gen/quent.hpp"

namespace {

namespace otel_sdk_logs = opentelemetry::sdk::logs;
namespace otel_exporter_logs = opentelemetry::exporter::logs;

struct QuillFrontendOptions : quill::FrontendOptions {
  static constexpr quill::QueueType queue_type = quill::QueueType::BoundedBlocking;
  static constexpr std::size_t initial_queue_capacity = 1024u * 1024u;
};

using QuillFrontend = quill::FrontendImpl<QuillFrontendOptions>;
using QuillLogger = quill::LoggerImpl<QuillFrontendOptions>;

class DiscardLogExporter final : public otel_sdk_logs::LogRecordExporter {
 public:
  std::unique_ptr<otel_sdk_logs::Recordable> MakeRecordable() noexcept override {
    return std::make_unique<otel_sdk_logs::ReadWriteLogRecord>();
  }

  opentelemetry::sdk::common::ExportResult Export(
      const opentelemetry::nostd::span<std::unique_ptr<otel_sdk_logs::Recordable>>& records)
      noexcept override {
    benchmark::DoNotOptimize(records.size());
    return opentelemetry::sdk::common::ExportResult::kSuccess;
  }

  bool ForceFlush(std::chrono::microseconds) noexcept override { return true; }
  bool Shutdown(std::chrono::microseconds) noexcept override { return true; }
};

std::string new_entity_id() {
  auto context = quent::Context::none();
  auto entity = context.entity_observer()->handle();
  return quent::to_string(entity.id().raw());
}

std::string export_directory() {
  const char* path = std::getenv("QUENT_BENCH_EXPORT_DIR");
  if (path == nullptr || path[0] == '\0') {
    throw std::runtime_error("QUENT_BENCH_EXPORT_DIR must be set for Quent");
  }
  return path;
}

std::string export_path(const std::string& filename) {
  return (std::filesystem::path(export_directory()) / filename).string();
}

void Clock(benchmark::State& benchmark_state) {
  for (auto _ : benchmark_state) {
    benchmark::DoNotOptimize(std::chrono::system_clock::now());
  }
}

void benchmark_quent(benchmark::State& benchmark_state, bool text_output) {
  auto context = text_output ? quent::Context::ndjson(export_path("quent"))
                             : quent::Context::none();
  auto entity = context.entity_observer()->handle();
  entity.tick();
  entity.tick();
  for (auto _ : benchmark_state) {
    entity.tick();
    benchmark::ClobberMemory();
  }
}

void QuentNoop(benchmark::State& benchmark_state) { benchmark_quent(benchmark_state, false); }

void QuentText(benchmark::State& benchmark_state) { benchmark_quent(benchmark_state, true); }

void benchmark_spdlog(benchmark::State& benchmark_state, bool text_output) {
  const auto entity_id = new_entity_id();
  std::shared_ptr<spdlog::sinks::sink> sink;
  if (text_output) {
    sink = std::make_shared<spdlog::sinks::basic_file_sink_st>(export_path("spdlog.log"), true);
  } else {
    sink = std::make_shared<spdlog::sinks::null_sink_st>();
  }
  spdlog::logger logger("quent-bench", std::move(sink));
  logger.set_level(spdlog::level::trace);
  logger.info("entity_id={}", entity_id);
  for (auto _ : benchmark_state) {
    logger.info("entity_id={}", entity_id);
  }
  logger.flush();
}

void SpdlogNoop(benchmark::State& benchmark_state) { benchmark_spdlog(benchmark_state, false); }

void SpdlogText(benchmark::State& benchmark_state) { benchmark_spdlog(benchmark_state, true); }

void benchmark_quill(benchmark::State& benchmark_state, quill::ClockSourceType clock_source,
                     const std::string& logger_name, bool text_output) {
  const auto entity_id = new_entity_id();
  quill::Backend::start();
  std::shared_ptr<quill::Sink> sink;
  if (text_output) {
    sink = QuillFrontend::create_or_get_sink<quill::JsonFileSink>(
        export_path(logger_name + ".json"), quill::FileSinkConfig{});
  } else {
    sink = QuillFrontend::create_or_get_sink<quill::NullSink>("quent-bench-null");
  }
  auto* logger = QuillFrontend::create_or_get_logger(
      logger_name, std::move(sink), quill::PatternFormatterOptions{}, clock_source);
  QuillFrontend::preallocate();
  LOG_INFO(logger, "entity_id={}", entity_id);
  logger->flush_log();
  std::size_t pending = 0;
  for (auto _ : benchmark_state) {
    LOG_INFO(logger, "entity_id={}", entity_id);
    if (++pending == 4096) {
      benchmark_state.PauseTiming();
      logger->flush_log();
      pending = 0;
      benchmark_state.ResumeTiming();
    }
  }
  logger->flush_log();
  quill::Backend::stop();
}

void QuillTscNoop(benchmark::State& benchmark_state) {
  benchmark_quill(benchmark_state, quill::ClockSourceType::Tsc, "quent-bench-tsc", false);
}

void QuillTscText(benchmark::State& benchmark_state) {
  benchmark_quill(benchmark_state, quill::ClockSourceType::Tsc, "quent-bench-tsc", true);
}

void QuillSystemNoop(benchmark::State& benchmark_state) {
  benchmark_quill(benchmark_state, quill::ClockSourceType::System, "quent-bench-system", false);
}

void QuillSystemText(benchmark::State& benchmark_state) {
  benchmark_quill(benchmark_state, quill::ClockSourceType::System, "quent-bench-system", true);
}

void benchmark_opentelemetry(benchmark::State& benchmark_state, bool text_output) {
  const auto entity_id = new_entity_id();
  std::ofstream output;
  std::unique_ptr<otel_sdk_logs::LogRecordExporter> exporter;
  if (text_output) {
    output.open(export_path("opentelemetry.log"), std::ios::out | std::ios::trunc);
    exporter = std::make_unique<otel_exporter_logs::OStreamLogRecordExporter>(output);
  } else {
    exporter = std::make_unique<DiscardLogExporter>();
  }
  auto processor = otel_sdk_logs::SimpleLogRecordProcessorFactory::Create(
      std::move(exporter));
  auto provider = otel_sdk_logs::LoggerProviderFactory::Create(std::move(processor));
  auto logger = provider->GetLogger("quent-bench", "");
  const auto emit = [&] {
    auto record = logger->CreateLogRecord();
    record->SetAttribute("entity.id", entity_id);
    logger->EmitLogRecord(std::move(record));
  };
  emit();
  for (auto _ : benchmark_state) {
    emit();
  }
  provider->Shutdown();
}

void OpenTelemetryNoop(benchmark::State& benchmark_state) {
  benchmark_opentelemetry(benchmark_state, false);
}

void OpenTelemetryText(benchmark::State& benchmark_state) {
  benchmark_opentelemetry(benchmark_state, true);
}

BENCHMARK(Clock)->Name("latency/cpp/noop/clock");
BENCHMARK(QuentNoop)->Name("latency/cpp/noop/quent");
BENCHMARK(SpdlogNoop)->Name("latency/cpp/noop/spdlog");
BENCHMARK(QuillTscNoop)->Name("latency/cpp/noop/quill-tsc");
BENCHMARK(QuillSystemNoop)->Name("latency/cpp/noop/quill-system");
BENCHMARK(OpenTelemetryNoop)->Name("latency/cpp/noop/opentelemetry");
BENCHMARK(Clock)->Name("latency/cpp/text/clock");
BENCHMARK(QuentText)->Name("latency/cpp/text/quent");
BENCHMARK(SpdlogText)->Name("latency/cpp/text/spdlog");
BENCHMARK(QuillTscText)->Name("latency/cpp/text/quill-tsc");
BENCHMARK(QuillSystemText)->Name("latency/cpp/text/quill-system");
BENCHMARK(OpenTelemetryText)->Name("latency/cpp/text/opentelemetry");

}  // namespace

int main(int argc, char** argv) {
  benchmark::Initialize(&argc, argv);
  if (benchmark::ReportUnrecognizedArguments(argc, argv)) {
    return 1;
  }
  benchmark::RunSpecifiedBenchmarks();
  benchmark::Shutdown();
  return 0;
}
