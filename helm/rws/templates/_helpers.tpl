{{/*
Expand the name of the chart.
*/}}
{{- define "rws.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name. Truncated to 63 chars since
some Kubernetes name fields are limited to this (by the DNS naming spec).
*/}}
{{- define "rws.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Chart name and version, used in the helm.sh/chart label.
*/}}
{{- define "rws.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels.
*/}}
{{- define "rws.labels" -}}
helm.sh/chart: {{ include "rws.chart" . }}
{{ include "rws.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels — kept separate from the common labels since selectors
are immutable after creation; only these should ever be used in a
`matchLabels` block.
*/}}
{{- define "rws.selectorLabels" -}}
app.kubernetes.io/name: {{ include "rws.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
The ServiceAccount name to use.
*/}}
{{- define "rws.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "rws.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}
