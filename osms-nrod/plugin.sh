case $1 in
   config)
        cat <<'EOM'
graph_title NTROD messages processed
graph_vlabel Messages processed
GotActivation_count.label Activation messages received
GotActivation_count.type DERIVE
GotActivation_count.min 0
DidActivation_count.label Activation messages processed
DidActivation_count.type DERIVE
DidActivation_count.min 0
GotCancellation_count.label Cancellation messages received
GotCancellation_count.type DERIVE
GotCancellation_count.min 0
DidCancellation_count.label Cancellation messages processed
DidCancellation_count.type DERIVE
DidCancellation_count.min 0
GotMovement_count.label Movement messages received
GotMovement_count.type DERIVE
GotMovement_count.min 0
DidMovement_count.label Movement messages processed
DidMovement_count.type DERIVE
DidMovement_count.min 0
EOM
        exit 0;;
esac

curl -s localhost:1234/metrics | sed s/_count/_count.value/g

