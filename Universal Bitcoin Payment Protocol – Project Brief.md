**UniversalBitcoinPaymentProtocol–** **Project** **Brief**

**Vision**

Today,Bitcoinhasmultiplepaymentrails:On-chain,Lightning,Ark,andmore.Eachonehasdifferent
trade-offs,forcinguserstodecidehowtheyshouldpay.

Ourgoalistoeliminatethatcomplexity.

WearebuildingaprotocolthatallowsuserstosendBitcoinusingasinglehuman-readableidentifier,
suchas:

> •alice@example.com •rodrigo@pay.dev •julian@bitcoin

InsteadofaskingthesendertochoosebetweenLightning,On-chain,orArk,theprotocolautomatically
discoverstherecipient'savailablepaymentmethodsandselectstheoptimalroute.

**Problem**
Today,sendingBitcoinrequiresuserstounderstandtechnicaldetails:

> •WhichaddressshouldIuse? •IsLightningavailable?
> •Areon-chainfeestoohigh?
>
> •DoestherecipientsupportArk? •Whichoptionischeaperandfaster?

Formostpeople,thisisconfusing.

**Solution**

Theprotocolresolvesahuman-readableidentifierintoacryptographicallysignedpaymentprofile.

Example:

> alice@example.com ↓
>
> Resolve Identity ↓
>
> Payment Profile ↓
>
> Lightning On-chain
>
> 1
>
> Ark
>
> Future protocols...

Then,aroutingengineevaluates:

> •Currentnetworkfees •Availablepaymentmethods •Paymentamount
> •Recipientcapabilities

Finally,itautomaticallyexecutesthebestpaymentroute.

**UniversalQR**

InsteadofgeneratingdifferentQRcodesfordifferentpaymentmethods,everyuserhasasingle
UniversalPaymentQR.

ScanningtheQRonlyprovidestheuser'sidentifier.

Example:

> bitcoinpay:alice@example.com

Thewalletresolvestheidentifier,retrievesthesignedpaymentprofile,andautomaticallyselectsthe
optimalpaymentrail.

TheQRneverneedstochange,eveniftheuserupdateswalletsorenablesnewpaymenttechnologies.

**Long-TermVision** WearenotreplacingexistingBitcoinstandards.

WearecreatingauniversalpaymentlayerthatmakeseveryBitcoinpaymentmethodaccessible
throughonesimpleidentitywhileremainingcompatiblewithexistingandfutureprotocols.

> 2
