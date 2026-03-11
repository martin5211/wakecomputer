"""AWS Lambda handler for WakeComputer

Translates Alexa Smart Home directives into REST calls to the Pi server.
"""

import json
import os
import uuid
from datetime import datetime, timezone
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

PI_BASE_URL = os.environ.get("PI_BASE_URL", "")
PI_API_TOKEN = os.environ.get("PI_API_TOKEN", "")


def lambda_handler(event, context):
    namespace = event["directive"]["header"]["namespace"]
    name = event["directive"]["header"]["name"]
    print(f"Directive: {namespace}/{name}")

    if namespace == "Alexa.Authorization" and name == "AcceptGrant":
        return make_response("Alexa.Authorization", "AcceptGrant.Response")

    if namespace == "Alexa.Discovery" and name == "Discover":
        return handle_discovery(event)

    if namespace == "Alexa.PowerController":
        return handle_power_controller(event)

    if namespace == "Alexa" and name == "ReportState":
        return handle_report_state(event)

    return make_error_response(event, "INVALID_DIRECTIVE", f"Unknown: {namespace}/{name}")


def handle_discovery(event):
    try:
        machines = pi_request("GET", "/machines")["machines"]
    except Exception as e:
        print(f"Discovery failed: {e}")
        return make_error_response(event, "ENDPOINT_UNREACHABLE", str(e))

    endpoints = []
    for machine_name in machines:
        endpoints.append({
            "endpointId": machine_name,
            "manufacturerName": "wakecomputer",
            "friendlyName": machine_name,
            "description": f"PC: {machine_name}",
            "displayCategories": ["COMPUTER"],
            "capabilities": [
                {
                    "type": "AlexaInterface",
                    "interface": "Alexa.PowerController",
                    "version": "3",
                    "properties": {
                        "supported": [{"name": "powerState"}],
                        "proactivelyReported": False,
                        "retrievable": True,
                    },
                },
                {
                    "type": "AlexaInterface",
                    "interface": "Alexa.EndpointHealth",
                    "version": "3",
                    "properties": {
                        "supported": [{"name": "connectivity"}],
                        "proactivelyReported": False,
                        "retrievable": True,
                    },
                },
                {
                    "type": "AlexaInterface",
                    "interface": "Alexa",
                    "version": "3",
                },
            ],
        })

    return {
        "event": {
            "header": {
                "namespace": "Alexa.Discovery",
                "name": "Discover.Response",
                "payloadVersion": "3",
                "messageId": str(uuid.uuid4()),
            },
            "payload": {"endpoints": endpoints},
        }
    }


def handle_report_state(event):
    directive = event["directive"]
    endpoint_id = directive["endpoint"]["endpointId"]

    try:
        result = pi_request("POST", "/status", {"machine": endpoint_id})
        power_state = "ON" if result.get("power") == "on" else "OFF"
        print(f"ReportState: machine={endpoint_id} power={power_state} result={result}")
    except Exception as e:
        print(f"ReportState failed: {e}")
        return make_error_response(event, "ENDPOINT_UNREACHABLE", str(e))

    response = {
        "event": {
            "header": {
                "namespace": "Alexa",
                "name": "StateReport",
                "payloadVersion": "3",
                "messageId": str(uuid.uuid4()),
                "correlationToken": directive["header"].get("correlationToken", ""),
            },
            "endpoint": {
                "endpointId": endpoint_id,
                "scope": directive["endpoint"].get("scope"),
            },
            "payload": {},
        },
        "context": {
            "properties": [
                {
                    "namespace": "Alexa.PowerController",
                    "name": "powerState",
                    "value": power_state,
                    "timeOfSample": _iso_now(),
                    "uncertaintyInMilliseconds": 2000,
                },
                {
                    "namespace": "Alexa.EndpointHealth",
                    "name": "connectivity",
                    "value": {"value": "OK"},
                    "timeOfSample": _iso_now(),
                    "uncertaintyInMilliseconds": 2000,
                },
            ]
        },
    }
    print(f"ReportState response: {json.dumps(response)}")
    return response


def handle_power_controller(event):
    directive = event["directive"]
    name = directive["header"]["name"]
    endpoint_id = directive["endpoint"]["endpointId"]

    try:
        if name == "TurnOn":
            pi_request("POST", "/wake", {"machine": endpoint_id})
        elif name == "TurnOff":
            pi_request("POST", "/shutdown", {"machine": endpoint_id})
        else:
            return make_error_response(event, "INVALID_DIRECTIVE", f"Unknown: {name}")
    except Exception as e:
        print(f"PowerController failed: {e}")
        return make_error_response(event, "ENDPOINT_UNREACHABLE", str(e))

    return {
        "event": {
            "header": {
                "namespace": "Alexa",
                "name": "Response",
                "payloadVersion": "3",
                "messageId": str(uuid.uuid4()),
                "correlationToken": directive["header"].get("correlationToken", ""),
            },
            "endpoint": {
                "endpointId": endpoint_id,
                "scope": directive["endpoint"].get("scope"),
            },
            "payload": {},
        },
        "context": {
            "properties": [
                {
                    "namespace": "Alexa.PowerController",
                    "name": "powerState",
                    "value": "ON" if name == "TurnOn" else "OFF",
                    "timeOfSample": _iso_now(),
                    "uncertaintyInMilliseconds": 0,
                },
                {
                    "namespace": "Alexa.EndpointHealth",
                    "name": "connectivity",
                    "value": {"value": "OK"},
                    "timeOfSample": _iso_now(),
                    "uncertaintyInMilliseconds": 0,
                },
            ]
        },
    }


def pi_request(method, path, body=None):
    url = PI_BASE_URL.rstrip("/") + path
    data = json.dumps(body).encode() if body else None
    req = Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {PI_API_TOKEN}")
    req.add_header("Content-Type", "application/json")

    try:
        with urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except HTTPError as e:
        raise RuntimeError(f"Pi returned {e.code}: {e.read().decode()}")
    except URLError as e:
        raise RuntimeError(f"Pi unreachable: {e.reason}")


def make_response(namespace, name):
    return {
        "event": {
            "header": {
                "namespace": namespace,
                "name": name,
                "payloadVersion": "3",
                "messageId": str(uuid.uuid4()),
            },
            "payload": {},
        }
    }


def _iso_now():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def make_error_response(event, error_type, message):
    return {
        "event": {
            "header": {
                "namespace": "Alexa",
                "name": "ErrorResponse",
                "payloadVersion": "3",
                "messageId": str(uuid.uuid4()),
            },
            "payload": {
                "type": error_type,
                "message": message,
            },
        }
    }
