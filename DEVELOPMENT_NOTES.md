# Developer Guide

## Transport Protocol

The transport protocol is implemented in a loop performing the following.

1. Listen for any incoming connections.

2. On connection, read initial bytes into a `TcpHeader` class. (source: [async_extension_host.py](https://github.com/opensearch-project/opensearch-sdk-py/blob/fbfbeef4d0dffbd6ecea32959ab4df5c1bf34431/src/opensearch_sdk_py/server/async_extension_host.py#L48))
   1. Identify whether it is a request or response, and what the request ID is. A response will carry the same ID.
   2. Identify the version of the sender, used in case compatibility decisions may change the response.
   3. Identify any thread context headers.
3. An [`OutboundMessageRequest`](https://github.com/opensearch-project/opensearch-sdk-py/blob/main/src/opensearch_sdk_py/transport/outbound_message_request.py) or `OutboundMessageResponse` subclass is instantiated, picking up reading the input stream.
   1. For requests, this instance reads the features and the name of the Transport Action identifying the `TransportMessage` handler.
   2. For responses, there is no additional information read, as the request ID identifies the handler expecting the response.
4. Following the fixed and variable headers, the content of the `TransportRequest` or `TransportResponse` is available for reading from the input stream. This stream and the instance created in the previous step are passed to the handler for the request (based on the action name) or response (based on the request ID).
5. Handlers parse the request from the input stream, perform whatever actions they need to perform, and then return a response as an outbound stream, matching the request ID in the case of requests. This outbound stream is then sent back to OpenSearch.
6. Sometimes the actions a handler performs are to send transport requests back to OpenSearch, where a similar loop will handle the request and return a response.

## Resources

* https://github.com/opensearch-project/opensearch-sdk-java/blob/main/CREATE_YOUR_FIRST_EXTENSION.md
* https://github.com/dbwiddis/CRUDExtension
