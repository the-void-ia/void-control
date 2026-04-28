import {
  toTemplateDetail,
  toTemplateDryRunResult,
  toTemplateExecutionResult,
  toTemplateSummary
} from "./models.js";

export class TemplatesClient {
  constructor(client) {
    this._client = client;
  }

  async list() {
    const payload = await this._client.getJson("/v1/templates");
    return (payload.templates ?? []).map(toTemplateSummary);
  }

  async get(templateId) {
    const payload = await this._client.getJson(`/v1/templates/${templateId}`);
    return toTemplateDetail(payload);
  }

  async dryRun(templateId, request) {
    const payload = await this._client.postJson(
      `/v1/templates/${templateId}/dry-run`,
      request
    );
    return toTemplateDryRunResult(payload);
  }

  async execute(templateId, request) {
    const payload = await this._client.postJson(
      `/v1/templates/${templateId}/execute`,
      request
    );
    return toTemplateExecutionResult(payload);
  }
}
