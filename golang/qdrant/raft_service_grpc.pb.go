// Code generated by protoc-gen-go-grpc. DO NOT EDIT.
// versions:
// - protoc-gen-go-grpc v1.2.0
// - protoc             v3.15.8
// source: raft_service.proto

package go_client

import (
	context "context"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
	emptypb "google.golang.org/protobuf/types/known/emptypb"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.32.0 or later.
const _ = grpc.SupportPackageIsVersion7

// RaftClient is the client API for Raft service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type RaftClient interface {
	// Send Raft message to another peer
	Send(ctx context.Context, in *RaftMessage, opts ...grpc.CallOption) (*emptypb.Empty, error)
	// Send to bootstrap peer
	// Returns uri by id if bootstrap knows this peer
	WhoIs(ctx context.Context, in *PeerId, opts ...grpc.CallOption) (*Uri, error)
	// Send to bootstrap peer
	// Proposes to add this peer Uri and ID to a map of all peers
	// Returns all peers
	AddPeerToKnown(ctx context.Context, in *AddPeerToKnownMessage, opts ...grpc.CallOption) (*AllPeers, error)
	// Send to bootstrap peer
	// Proposes to add this peer as participant of consensus
	AddPeerAsParticipant(ctx context.Context, in *PeerId, opts ...grpc.CallOption) (*emptypb.Empty, error)
}

type raftClient struct {
	cc grpc.ClientConnInterface
}

func NewRaftClient(cc grpc.ClientConnInterface) RaftClient {
	return &raftClient{cc}
}

func (c *raftClient) Send(ctx context.Context, in *RaftMessage, opts ...grpc.CallOption) (*emptypb.Empty, error) {
	out := new(emptypb.Empty)
	err := c.cc.Invoke(ctx, "/qdrant.Raft/Send", in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *raftClient) WhoIs(ctx context.Context, in *PeerId, opts ...grpc.CallOption) (*Uri, error) {
	out := new(Uri)
	err := c.cc.Invoke(ctx, "/qdrant.Raft/WhoIs", in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *raftClient) AddPeerToKnown(ctx context.Context, in *AddPeerToKnownMessage, opts ...grpc.CallOption) (*AllPeers, error) {
	out := new(AllPeers)
	err := c.cc.Invoke(ctx, "/qdrant.Raft/AddPeerToKnown", in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *raftClient) AddPeerAsParticipant(ctx context.Context, in *PeerId, opts ...grpc.CallOption) (*emptypb.Empty, error) {
	out := new(emptypb.Empty)
	err := c.cc.Invoke(ctx, "/qdrant.Raft/AddPeerAsParticipant", in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// RaftServer is the server API for Raft service.
// All implementations must embed UnimplementedRaftServer
// for forward compatibility
type RaftServer interface {
	// Send Raft message to another peer
	Send(context.Context, *RaftMessage) (*emptypb.Empty, error)
	// Send to bootstrap peer
	// Returns uri by id if bootstrap knows this peer
	WhoIs(context.Context, *PeerId) (*Uri, error)
	// Send to bootstrap peer
	// Proposes to add this peer Uri and ID to a map of all peers
	// Returns all peers
	AddPeerToKnown(context.Context, *AddPeerToKnownMessage) (*AllPeers, error)
	// Send to bootstrap peer
	// Proposes to add this peer as participant of consensus
	AddPeerAsParticipant(context.Context, *PeerId) (*emptypb.Empty, error)
	mustEmbedUnimplementedRaftServer()
}

// UnimplementedRaftServer must be embedded to have forward compatible implementations.
type UnimplementedRaftServer struct {
}

func (UnimplementedRaftServer) Send(context.Context, *RaftMessage) (*emptypb.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Send not implemented")
}
func (UnimplementedRaftServer) WhoIs(context.Context, *PeerId) (*Uri, error) {
	return nil, status.Errorf(codes.Unimplemented, "method WhoIs not implemented")
}
func (UnimplementedRaftServer) AddPeerToKnown(context.Context, *AddPeerToKnownMessage) (*AllPeers, error) {
	return nil, status.Errorf(codes.Unimplemented, "method AddPeerToKnown not implemented")
}
func (UnimplementedRaftServer) AddPeerAsParticipant(context.Context, *PeerId) (*emptypb.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method AddPeerAsParticipant not implemented")
}
func (UnimplementedRaftServer) mustEmbedUnimplementedRaftServer() {}

// UnsafeRaftServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to RaftServer will
// result in compilation errors.
type UnsafeRaftServer interface {
	mustEmbedUnimplementedRaftServer()
}

func RegisterRaftServer(s grpc.ServiceRegistrar, srv RaftServer) {
	s.RegisterService(&Raft_ServiceDesc, srv)
}

func _Raft_Send_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(RaftMessage)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(RaftServer).Send(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/qdrant.Raft/Send",
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(RaftServer).Send(ctx, req.(*RaftMessage))
	}
	return interceptor(ctx, in, info, handler)
}

func _Raft_WhoIs_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(PeerId)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(RaftServer).WhoIs(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/qdrant.Raft/WhoIs",
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(RaftServer).WhoIs(ctx, req.(*PeerId))
	}
	return interceptor(ctx, in, info, handler)
}

func _Raft_AddPeerToKnown_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AddPeerToKnownMessage)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(RaftServer).AddPeerToKnown(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/qdrant.Raft/AddPeerToKnown",
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(RaftServer).AddPeerToKnown(ctx, req.(*AddPeerToKnownMessage))
	}
	return interceptor(ctx, in, info, handler)
}

func _Raft_AddPeerAsParticipant_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(PeerId)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(RaftServer).AddPeerAsParticipant(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: "/qdrant.Raft/AddPeerAsParticipant",
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(RaftServer).AddPeerAsParticipant(ctx, req.(*PeerId))
	}
	return interceptor(ctx, in, info, handler)
}

// Raft_ServiceDesc is the grpc.ServiceDesc for Raft service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var Raft_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "qdrant.Raft",
	HandlerType: (*RaftServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "Send",
			Handler:    _Raft_Send_Handler,
		},
		{
			MethodName: "WhoIs",
			Handler:    _Raft_WhoIs_Handler,
		},
		{
			MethodName: "AddPeerToKnown",
			Handler:    _Raft_AddPeerToKnown_Handler,
		},
		{
			MethodName: "AddPeerAsParticipant",
			Handler:    _Raft_AddPeerAsParticipant_Handler,
		},
	},
	Streams:  []grpc.StreamDesc{},
	Metadata: "raft_service.proto",
}
